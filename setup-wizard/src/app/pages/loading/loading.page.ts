import { Component } from '@angular/core'
import { iosTransitionAnimation, NavController } from '@ionic/angular'
import { StateService } from 'src/app/services/state.service'

@Component({
  selector: 'app-loading',
  templateUrl: 'loading.page.html',
  styleUrls: ['loading.page.scss'],
})
export class LoadingPage {
  constructor(
    private stateService: StateService,
    private navCtrl: NavController
  ) {}

  ngOnInit () {
    this.stateService.pollDataTransferProgress()
    const progSub = this.stateService.dataProgSubject.subscribe(async progress => {
      if(progress === 1) {
        progSub.unsubscribe()
        await this.navCtrl.navigateForward(`/success`, { animationDirection: 'forward', animation: iosTransitionAnimation })
      }
    })
  }

}

